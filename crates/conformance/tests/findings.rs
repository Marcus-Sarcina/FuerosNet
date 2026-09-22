//! Current specification assertions. Passing checks confirm previous repairs;
//! failing checks reproduce current findings against unmodified source files.
use rhtn_archive::{record::{Record, SigStatus}, topology::{Table, Evaluation}, tx::*};
use rhtn_codec::{cbor::*, cose, encode::*};
use rhtn_crypto::{identity::testkit::test_identity, SigningIdentity};
use rhtn_node::view::NodeView;
use std::sync::Arc;

struct Missing;
impl rhtn_archive::walk::Fetch for Missing {
    fn fetch(&self, _: &[u8; 32]) -> Option<Vec<u8>> { None }
}
fn replace(b: &[u8], key: u64, value: &[u8]) -> Vec<u8> {
    let r = value_slice(b, key).unwrap();
    [b[..r.start].to_vec(), value.to_vec(), b[r.end..].to_vec()].concat()
}
fn adoption(n: &SigningIdentity, p: &SigningIdentity, evidence: Evidence) -> Vec<u8> {
    let bn = [cose::sha256(&n.public.keyhash)];
    let bp = [cose::sha256(&p.public.keyhash)];
    adoption_body(&Adoption {
        node: n.public.keyhash, patron: p.public.keyhash,
        locator: Locator { anchor: p.public.keyhash, path: vec![0], nibbles: 1, seqno: Seqno { series: 1, counter: 0 } },
        timestamp: 100, key_material: None, evidence, presented_head: None, back: [&bn, &bp],
    })
}
fn recovery(n: &SigningIdentity, p: &SigningIdentity, old: &SigningIdentity, response_subject: &SigningIdentity) -> Vec<u8> {
    let response = recovery_response(p, response_subject, &[7; 32], &old.public.keyhash);
    let mut statement = vec![];
    emit_array_head(&mut statement, 3);
    for k in [&old.public.keyhash, &n.public.keyhash, &p.public.keyhash] { emit_bstr(&mut statement, k); }
    let mut block = vec![];
    emit_map_head(&mut block, 3);
    emit_uint(&mut block, 1); emit_bstr(&mut block, &old.public.keyhash);
    emit_uint(&mut block, 2); emit_array_head(&mut block, 1); block.extend(response);
    emit_uint(&mut block, 3); block.extend(sign_block(old, cose::aad::SUCCESSOR, &statement));
    envelope(TYPE_ADOPTION, &adoption(n, p, Evidence::Recovery(block)), &[n, p])
}

#[test]
fn valid_transfer_control() {
    let (n, p, old) = (test_identity("alice"), test_identity("bob"), test_identity("carol"));
    let body = adoption(&n, &p, Evidence::Transfer { former: old.public.keyhash, block: transfer_block(&old, &n.public.keyhash, &p.public.keyhash) });
    let rec = Record::parse(&envelope(TYPE_ADOPTION, &body, &[&n, &p])).unwrap();
    assert_eq!(rec.check_signatures(&vec![n.public, p.public, old.public]), SigStatus::Verified);
}

#[test]
fn f01_empty_transfer_signature_must_be_rejected() {
    let (n, p, old) = (test_identity("alice"), test_identity("bob"), test_identity("carol"));
    // COSE_Sign = [h'', {}, nil, []]: no signature from the former patron.
    let body = adoption(&n, &p, Evidence::Transfer { former: old.public.keyhash, block: vec![0x84, 0x40, 0xa0, 0xf6, 0x80] });
    let rec = Record::parse(&envelope(TYPE_ADOPTION, &body, &[&n, &p])).unwrap();
    assert_ne!(rec.check_signatures(&vec![n.public, p.public, old.public]), SigStatus::Verified,
        "wire §4.1 and §3.5 require both former-patron signatures");
}

#[test]
fn f02_adoption_without_evidence_must_be_rejected() {
    let (n, p) = (test_identity("alice"), test_identity("bob"));
    let body = map_without_key(&adoption(&n, &p, Evidence::Presence([1; 32])), 8).unwrap();
    let bytes = envelope(TYPE_ADOPTION, &body, &[&n, &p]);
    assert!(Record::parse(&bytes).is_err(), "wire §4.1 requires exactly one of 6, 8, 9");
}

#[test]
fn f02_recovery_response_for_another_successor_must_be_rejected() {
    let (n, p, old, other) = (test_identity("alice2"), test_identity("bob"), test_identity("alice"), test_identity("carol"));
    let bytes = recovery(&n, &p, &old, &other);
    let ids = vec![n.public, p.public, old.public, other.public];
    let accepted = Record::parse(&bytes).is_ok_and(|r| r.check_signatures(&ids) == SigStatus::Verified);
    assert!(!accepted, "wire §4.1 binds every response subject to adoption field 1");
}

#[test]
fn f02_non_genesis_formation_must_be_rejected() {
    let (n, p) = (test_identity("alice"), test_identity("bob"));
    let body = formation_body([&[[1;32]], &[[2;32]]], [&n.public.keyhash, &p.public.keyhash], 100, 101, &[3;32]);
    assert!(Record::parse(&envelope(TYPE_PRESENCE, &body, &[&n, &p])).is_err(),
        "wire §3.2 requires genesis back-pointers for formation");
}

#[test]
fn f03_embedded_envelope_payload_must_be_rejected() {
    let (n, p) = (test_identity("alice"), test_identity("bob"));
    let body = adoption(&n, &p, Evidence::Presence([1; 32]));
    let env = envelope(TYPE_ADOPTION, &body, &[&n, &p]);
    let sig_range = value_slice(&env, 4).unwrap();
    let mut sig = env[sig_range].to_vec();
    let payload = array_item_ranges(&sig, 0).unwrap()[2].clone();
    sig.splice(payload, [0x41, 0x00]); // h'00' instead of nil, signatures unchanged
    let bad = replace(&env, 4, &sig);
    assert!(rhtn_crypto::verify::envelope(&vec![n.public, p.public], &bad).is_err(),
        "wire §1 requires detached nil payloads");
}

#[test]
fn f04_departure_received_before_adoption_must_still_end_binding() {
    let (n, p, old) = (test_identity("alice"), test_identity("bob"), test_identity("carol"));
    let body = adoption(&n, &p, Evidence::Transfer { former: old.public.keyhash, block: transfer_block(&old, &n.public.keyhash, &p.public.keyhash) });
    let a = Record::parse(&envelope(TYPE_ADOPTION, &body, &[&n, &p])).unwrap();
    let body = departure_body(&[a.txid], &n.public.keyhash, &p.public.keyhash, Seqno { series: 1, counter: 1 }, 101, None);
    let d = Record::parse(&envelope(TYPE_DEPARTURE, &body, &[&n])).unwrap();
    let ids = vec![n.public.clone(), p.public.clone(), old.public];
    let mut t = Table::with_me(p.public.keyhash);
    for r in [&d, &a] { t.apply_with(r, &ids, &Missing, None, Evaluation::Deferred).unwrap(); }
    assert!(!t.patrons(&n.public.keyhash).contains(&p.public.keyhash), "reordered delivery must not resurrect a departed binding");
}

#[test]
fn f09_queue_restart_in_same_second_must_not_overwrite() {
    use rhtn_node::queue::{DirStore, QueueStore, Queued};
    let dir = std::env::temp_dir().join(format!("rhtn-review-queue-{}", std::process::id()));
    let first = DirStore::new(&dir);
    first.push(Queued { recipient: [7;32], arrival: 100, ciphertext: vec![1] });
    drop(first);
    let second = DirStore::new(&dir);
    second.push(Queued { recipient: [7;32], arrival: 100, ciphertext: vec![2] });
    let got = second.list(&[7;32]);
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(got.len(), 2, "accepted queued messages survive a restart in their arrival second");
}

#[tokio::test]
async fn f05_live_recovery_must_revoke_transport_credential() {
    use rhtn_node::{runtime::LiveNode, resolution::{AnchorTable, Ingestion}};
    use rhtn_transport::{session::NodeConfig, tls::Pins};
    let (n, p, old) = (test_identity("alice2"), test_identity("bob"), test_identity("alice"));
    let bytes = recovery(&n, &p, &old, &n);
    let ids = vec![n.public.clone(), p.public.clone(), old.public.clone()];
    let p = Arc::new(p);
    let view = NodeView::new(p.clone(), Locator::root(p.public.keyhash, Seqno { series: 1, counter: 0 }));
    let live = LiveNode::start(NodeConfig::defaults(p, Pins::new(), 30), view, ids, AnchorTable::new(0, Ingestion::UnverifiedGossip));
    assert_eq!(live.originate_transaction(&bytes), rhtn_node::store::Decision::Stored);
    assert!(live.view.lock().unwrap().is_superseded(&old.public.keyhash), "control: topology knows recovery");
    assert!(live.node.is_superseded(&old.public.keyhash), "infra §2 requires serving state to revoke the old credential too");
}

#[derive(Default)]
struct Capture {
    peers: Vec<[u8;32]>,
    controls: std::sync::Mutex<Vec<u64>>,
    requests: std::sync::Mutex<Vec<u64>>,
}
impl rhtn_node::Adjacency for Capture {
    fn peers(&self) -> Vec<[u8;32]> { self.peers.clone() }
    fn send(&self, _: &[u8;32], t: u64, _: &[u8]) { self.controls.lock().unwrap().push(t); }
    fn request(&self, _: &[u8;32], t: u64, _: &[u8]) -> bool { self.requests.lock().unwrap().push(t); true }
}
#[test]
fn f06_currency_fallback_must_open_request_stream() {
    let n = Arc::new(test_identity("alice"));
    let p = test_identity("bob");
    let mut view = NodeView::new(n.clone(), Locator::root(n.public.keyhash, Seqno { series: 1, counter: 0 }));
    let adj = Capture { peers: vec![p.public.keyhash], ..Default::default() };
    assert!(view.ask_currency(&adj, [9;32], Some(p.public.keyhash), None, [1;16]).is_some());
    assert!(adj.controls.lock().unwrap().is_empty());
    assert_eq!(*adj.requests.lock().unwrap(), vec![8]);
}

#[test]
fn f07_short_packed_path_must_be_rejected_before_indexing() {
    use rhtn_node::resolution::ResolveRequest;
    let req = ResolveRequest { subject: [1;32], anchor: [2;32], path: vec![], nibbles: 1, nonce: [3;16] };
    let outcome = std::panic::catch_unwind(|| {
        let decoded = ResolveRequest::decode(&req.encode());
        if let Ok(r) = &decoded { let _ = r.path().indices(); }
        decoded.is_err()
    });
    assert!(matches!(outcome, Ok(true)), "wire §2.1 requires ceil(nibbles/2) bytes; malformed path must be rejected without panic");
}

#[tokio::test]
async fn f11_live_node_must_use_a_running_clock() {
    use rhtn_node::{runtime::LiveNode, resolution::{AnchorTable, Ingestion}};
    use rhtn_transport::{session::NodeConfig, tls::Pins};
    use rhtn_node::currency::{CurrencyRequest, CurrencyReply};
    let (n, old) = (test_identity("alice"), test_identity("carol"));
    let p = Arc::new(test_identity("bob"));
    let mut view = NodeView::new(p.clone(), Locator::root(p.public.keyhash, Seqno { series: 1, counter: 0 }));
    view.set_now(100);
    let ids = vec![n.public.clone(), p.public.clone(), old.public.clone()];
    let body = adoption(&n, &p, Evidence::Transfer { former: old.public.keyhash, block: transfer_block(&old, &n.public.keyhash, &p.public.keyhash) });
    let rec = Record::parse(&envelope(TYPE_ADOPTION, &body, &[&n, &p])).unwrap();
    view.table.apply(&rec, &ids, &Missing, None).unwrap();
    let mut cfg = NodeConfig::defaults(p.clone(), Pins::new(), 30);
    let clock = Arc::new(std::sync::atomic::AtomicU64::new(100));
    let clock_read = clock.clone();
    cfg.clock = Arc::new(move || clock_read.load(std::sync::atomic::Ordering::SeqCst));
    let live = LiveNode::start(cfg, view, ids.clone(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    clock.store(101, std::sync::atomic::Ordering::SeqCst);
    let req = CurrencyRequest { subject: n.public.keyhash, nonce: [7;16] };
    let bytes = live.node.cfg.on_request.as_ref().unwrap()(n.public.keyhash, rhtn_codec::schema::Family::CurrencyRequest, req.encode()).await.unwrap();
    let CurrencyReply::Attestation { bytes, .. } = CurrencyReply::decode(&bytes).unwrap() else { panic!("patron must issue") };
    let att = rhtn_archive::currency::parse_attestation(&ids, &bytes).unwrap();
    assert_eq!(att.issued_at, 101, "currency issuance must read current clock rather than the view's setup timestamp");
}

#[test]
fn f02_adoption_with_malformed_locator_must_be_rejected() {
    let (n,p) = (test_identity("alice"), test_identity("bob"));
    let body = adoption(&n,&p,Evidence::Presence([1;32]));
    let loc = value_slice(&body,3).unwrap();
    // Empty path bytes paired with a declared length of one nibble.
    let invalid = Locator { anchor:p.public.keyhash,path:vec![],nibbles:1,seqno:Seqno{series:1,counter:0} };
    let mut encoded = vec![]; invalid.emit(&mut encoded);
    let body = [body[..loc.start].to_vec(),encoded,body[loc.end..].to_vec()].concat();
    let raw = envelope(TYPE_ADOPTION,&body,&[&n,&p]);
    let accepted = Record::parse(&raw).is_ok_and(|r| r.check_signatures(&vec![n.public,p.public]) == SigStatus::Verified);
    assert!(!accepted,"wire §§2.1,4.1: a signed adoption must validate its locator's packed path");
}

#[test]
fn f02_witness_nominator_must_be_a_participant() {
    let (n,p,w) = (test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let backs = [&n,&p,&w].iter().map(|i| vec![cose::sha256(&i.public.keyhash)]).collect::<Vec<_>>();
    let body = presence_record_body(&backs,[&n.public.keyhash,&p.public.keyhash],&[Witness{keyhash:w.public.keyhash,nominated_by:w.public.keyhash,flags:3}],100,101,&[3;32]);
    let raw = envelope(TYPE_PRESENCE,&body,&[&n,&p,&w]);
    let accepted = Record::parse(&raw).is_ok_and(|r| r.check_signatures(&vec![n.public,p.public,w.public]) == SigStatus::Verified);
    assert!(!accepted,"wire §3.2: a witness's nominated_by MUST be a participant");
}

#[test]
fn n08_sign1_with_wrong_algorithm_must_be_rejected() {
    let n = test_identity("alice");
    let good = currency_attestation(&n,&n.public.keyhash,&n.public.keyhash,100,200,0);
    let payload = map_without_key(&good,7).unwrap();
    let prot = cose::protected_alg(-49); // claims ML-DSA, but Ed25519 signs
    let sig = n.sign_ed(&cose::sig_structure_sign1(&prot,cose::aad::CURRENCY,&payload));
    let mut sign1=vec![]; emit_array_head(&mut sign1,4); emit_bstr(&mut sign1,&prot); emit_map_head(&mut sign1,0); emit_null(&mut sign1); emit_bstr(&mut sign1,&sig);
    let bad = replace(&good,7,&sign1);
    assert!(!matches!(rhtn_crypto::verify::record(&vec![n.public],"CurrencyAttestation",&bad),Ok(())),"wire §§3.5,7.1: standalone classical signature must declare alg -8");
}

#[test]
fn f04_departure_before_adoption_must_not_restore_routing_slot() {
    use rhtn_node::store::{Decision,KIND_TRANSACTION};
    let (n,p,old) = (test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"));
    let a = Record::parse(&envelope(TYPE_ADOPTION,&adoption(&n,&p,Evidence::Transfer{former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash)}),&[&n,&p])).unwrap();
    let d = envelope(TYPE_DEPARTURE,&departure_body(&[a.txid],&n.public.keyhash,&p.public.keyhash,Seqno{series:1,counter:1},101,None),&[&n]);
    let mut view = NodeView::new(p.clone(),Locator::root(p.public.keyhash,Seqno{series:2,counter:0}));
    let ids=vec![n.public.clone(),p.public.clone(),old.public];
    for raw in [&d,&a.bytes] { assert_eq!(view.take_object(&Capture::default(),&p.public.keyhash,KIND_TRANSACTION,raw,&ids),Decision::Stored); }
    assert!(view.table.patrons(&n.public.keyhash).is_empty(),"control: binding stays closed");
    assert!(view.slots.values().all(|s|s.occupant!=Some(n.public.keyhash)),"infra §4: a departed child must not reappear in routing slots");
}

struct All;
impl rhtn_node::store::Horizon for All { fn within(&self,_:&[u8;32],_:usize)->bool{true} }

#[test]
fn n01_endpoint_without_signature_must_be_rejected_with_known_key() {
    use rhtn_node::{store::*,resolution::*};
    let n=test_identity("alice");
    let good=endpoint_record(&n,&[NetworkPoint::from_socket("127.0.0.1:9000".parse().unwrap()).unwrap()],Seqno{series:1,counter:1});
    let bad=map_without_key(&good,4).unwrap();
    let mut st=TopologyStore::new();
    let got=st.accept(KIND_ENDPOINT_RECORD,&bad,&n.public.keyhash,&vec![n.public.clone()],&All);
    assert!(matches!(got,Decision::Malformed(_)),"wire §§7.6,10.1.2: missing signature is malformed, not unverified gossip; got {got:?}");
}

#[test]
fn n02_retired_endpoint_must_not_return_after_restart() {
    use rhtn_node::{store::*,resolution::*};
    let n=test_identity("alice"); let ids=vec![n.public.clone()];
    let sq=Seqno{series:1,counter:1};
    let ep=|port|endpoint_record(&n,&[NetworkPoint::from_socket(format!("127.0.0.1:{port}").parse().unwrap()).unwrap()],sq);
    let dir=std::env::temp_dir().join(format!("rhtn-review-conflict-{}",std::process::id()));
    let mut st=TopologyStore::new();
    assert_eq!(st.accept(KIND_ENDPOINT_RECORD,&ep(9001),&n.public.keyhash,&ids,&All),Decision::Stored);
    st.save(&dir).unwrap();
    assert!(matches!(st.accept(KIND_ENDPOINT_RECORD,&ep(9002),&n.public.keyhash,&ids,&All),Decision::Conflict{..}));
    assert!(st.endpoint(&n.public.keyhash).is_none());
    st.save(&dir).unwrap();
    let restored=TopologyStore::load(&dir).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(restored.conflicted(&n.public.keyhash,sq),"control: conflict marker persists");
    assert!(restored.endpoint(&n.public.keyhash).is_none(),"wire §10.1.2: neither conflicting endpoint is current after a restart");
}

#[test]
fn n03_missing_embedded_signer_key_must_be_unverifiable() {
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let a=envelope(TYPE_ADOPTION,&adoption(&n,&p,Evidence::Transfer{former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash)}),&[&n,&p]);
    let rec=Record::parse(&a).unwrap();
    assert_eq!(rec.check_signatures(&vec![n.public,p.public]),SigStatus::Unverifiable{missing:old.public.keyhash},"wire §3.4: name the missing former-patron key so the object can be held and retried");
}

#[test]
fn n04_replaying_archived_predecessor_must_not_create_a_fork() {
    use rhtn_archive::chain::Archive;
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let a=Record::parse(&envelope(TYPE_ADOPTION,&adoption(&n,&p,Evidence::Transfer{former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash)}),&[&n,&p])).unwrap();
    let b=Record::parse(&envelope(TYPE_DEPARTURE,&departure_body(&[a.txid],&n.public.keyhash,&p.public.keyhash,Seqno{series:1,counter:1},101,None),&[&n])).unwrap();
    let mut ar=Archive::new(n.public.keyhash);
    ar.append(a.clone()).unwrap(); ar.append(b.clone()).unwrap(); ar.append(a).unwrap();
    assert_eq!(ar.heads(),vec![b.txid],"design §10.3: receiving the same predecessor twice must not create a second branch head");
}

#[test]
fn n05_pins_must_require_both_identity_components() {
    let n=test_identity("alice"); let km=n.public.key_material();
    let first=array_item_ranges(&km,0).unwrap()[0].clone();
    let mut bad=vec![];emit_array_head(&mut bad,1);bad.extend_from_slice(&km[first]);
    let kh=cose::sha256(&bad); let pins=rhtn_transport::tls::Pins::new();
    let accepted=pins.pin(kh,&bad).is_ok() && pins.classical_key(&kh).is_some();
    assert!(!accepted,"wire §§2.2,9.1: a classical-only KeyMaterial array must not become a usable hybrid identity pin");
}

#[tokio::test]
async fn n06_fragmented_control_frame_survives_an_outbound_event() {
    use rhtn_transport::{session::*,tls::{self,Pins}};
    use tokio::time::{Duration,sleep,timeout};
    timeout(Duration::from_secs(5),async {
        let (n,p)=(test_identity("alice"),Arc::new(test_identity("bob")));
        let pins=Pins::new(); pins.pin_identity(&n.public); pins.pin_identity(&p.public);
        let mut cfg=NodeConfig::defaults(p.clone(),pins.clone(),3600);cfg.log=Log::recording();
        let ep=tls::server_endpoint(&p,"127.0.0.1:0".parse().unwrap()).unwrap();
        let addr=ep.local_addr().unwrap();let node=Node::new(cfg);
        let server=tokio::spawn(node.clone().serve(ep.clone()));
        let client=tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let conn=tls::dial(&client,&n,&pins,&p.public.keyhash,addr).unwrap().await.unwrap();
        let (mut send,mut recv)=conn.open_bi().await.unwrap();
        send.write_all(&control_frame(FRAME_ATTACH,&encode_attach(&n.public.keyhash,None,&Default::default()))).await.unwrap();
        assert!(matches!(read_frame(&mut recv,1_000_000).await,FrameRead::Payload(_)));
        send.write_all(&control_frame(FRAME_HEARTBEAT,&encode_heartbeat(0,100))).await.unwrap();
        sleep(Duration::from_millis(80)).await;
        assert_eq!(node.log.count(|e|matches!(e,Event::Received{frame_type:FRAME_HEARTBEAT})),1,"control: complete frame is read");
        let frame=control_frame(FRAME_HEARTBEAT,&encode_heartbeat(1,101));
        send.write_all(&frame[..4]).await.unwrap();
        sleep(Duration::from_millis(80)).await;
        assert!(node.send_control(&n.public.keyhash,99,&[0xa0]));
        assert!(matches!(read_frame(&mut recv,1_000_000).await,FrameRead::Payload(_)),"outbound event was processed");
        send.write_all(&frame[4..]).await.unwrap();
        sleep(Duration::from_millis(100)).await;
        let got=node.log.count(|e|matches!(e,Event::Received{frame_type:FRAME_HEARTBEAT}));
        conn.close(0u32.into(),b"review complete"); ep.close(0u32.into(),b"review complete");server.abort();
        assert_eq!(got,2,"wire §§8.2,9.2: interleaving an outbound event must not lose a partially received frame");
    }).await.expect("bounded test");
}

#[test]
fn n07_concurrent_enqueue_must_enforce_the_recipient_cap() {
    use rhtn_transport::{session::{Node,NodeConfig},tls::Pins};
    let mut cfg=NodeConfig::defaults(Arc::new(test_identity("bob")),Pins::new(),30);
    cfg.queue_cap=Some(1);
    let node=Node::new(cfg);
    // Synchronise callers before enqueue, not inside QueueStore::bytes:
    // the repaired implementation intentionally serialises that method.
    let start=std::sync::Barrier::new(2);
    let results=std::thread::scope(|s| {
        let a=s.spawn(||{start.wait();node.enqueue([7;32],vec![1])});
        let b=s.spawn(||{start.wait();node.enqueue([7;32],vec![2])});
        [a.join().unwrap(),b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r|r.is_ok()).count(),1);
    assert_eq!(node.queued(&[7;32]),1);
}

#[test]
fn f02_public_crypto_verifier_must_enforce_body_rules() {
    let (n,p)=(test_identity("alice"),test_identity("bob"));
    let body=map_without_key(&adoption(&n,&p,Evidence::Presence([1;32])),8).unwrap();
    let raw=envelope(TYPE_ADOPTION,&body,&[&n,&p]);
    assert!(Record::parse(&raw).is_err(),"control: archive parser rejects absent evidence");
    assert!(rhtn_crypto::verify::envelope(&vec![n.public,p.public],&raw).is_err(),"wire §3.4: the public envelope verifier must enforce the same locally checkable body rules");
}

#[test]
fn n09_presence_evaluation_must_verify_the_fetched_records_signatures() {
    let (n,p)=(test_identity("alice"),test_identity("bob"));
    let body=formation_body([&[cose::sha256(&n.public.keyhash)],&[cose::sha256(&p.public.keyhash)]],[&n.public.keyhash,&p.public.keyhash],90,91,&[3;32]);
    let mut pop=envelope(TYPE_PRESENCE,&body,&[&n,&p]);
    let ids=vec![n.public.clone(),p.public.clone()];
    assert_eq!(Record::parse(&pop).unwrap().check_signatures(&ids),SigStatus::Verified);
    *pop.last_mut().unwrap()^=1;
    let pr=Record::parse(&pop).unwrap();
    assert!(matches!(pr.check_signatures(&ids),SigStatus::Invalid(_)),"control: referenced evidence has an invalid signature");
    struct One(Vec<u8>);
    impl rhtn_archive::walk::Fetch for One{fn fetch(&self,_:&[u8;32])->Option<Vec<u8>>{Some(self.0.clone())}}
    let a=Record::parse(&envelope(TYPE_ADOPTION,&adoption(&n,&p,Evidence::Presence(pr.txid)),&[&n,&p])).unwrap();
    let mut table=Table::with_me(p.public.keyhash);
    assert!(table.apply(&a,&ids,&One(pop),None).is_err(),"wire §§3.4,4.1: invalid fetched signatures must not satisfy required presence evaluation");
}

#[test]
fn f02_transfer_signer_must_differ_from_adoption_participants() {
    let (n,p)=(test_identity("alice"),test_identity("bob"));
    let mut outcomes=Vec::new();
    for former in [&n,&p] {
        let body=adoption(&n,&p,Evidence::Transfer { former:former.public.keyhash, block:transfer_block(former,&n.public.keyhash,&p.public.keyhash) });
        let raw=envelope(TYPE_ADOPTION,&body,&[&n,&p]);
        let accepted=Record::parse(&raw).is_ok_and(|r|r.check_signatures(&vec![n.public.clone(),p.public.clone()])==SigStatus::Verified);
        outcomes.push(accepted);
    }
    assert_eq!(outcomes,vec![false,false],"wire §4.1: former patron must differ from both adoption participants");
}

#[test]
fn f04_old_series_departure_must_not_clear_a_new_relationship_slot() {
    use rhtn_node::store::{Decision,KIND_TRANSACTION};
    let (n,p,old)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"));
    let first=adoption(&n,&p,Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) });
    let a=Record::parse(&envelope(TYPE_ADOPTION,&first,&[&n,&p])).unwrap();
    let departed=envelope(TYPE_DEPARTURE,&departure_body(&[a.txid],&n.public.keyhash,&p.public.keyhash,Seqno{series:1,counter:1},101,None),&[&n]);
    let d=Record::parse(&departed).unwrap();
    let bn=[d.txid];let bp=[a.txid];
    let later=adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator{anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno{series:2,counter:0}},timestamp:200,key_material:None,evidence:Evidence::Presence([8;32]),presented_head:None,back:[&bn,&bp] });
    let a2=envelope(TYPE_ADOPTION,&later,&[&n,&p]);
    let mut view=NodeView::new(p.clone(),Locator::root(p.public.keyhash,Seqno{series:3,counter:0}));
    let ids=vec![n.public.clone(),p.public.clone(),old.public];
    for raw in [&a.bytes,&a2,&departed] { assert_eq!(view.take_object(&Capture::default(),&p.public.keyhash,KIND_TRANSACTION,raw,&ids),Decision::Stored); }
    assert!(view.table.patrons(&n.public.keyhash).contains(&p.public.keyhash),"control: new series remains bound");
    assert_eq!(view.slot_of(&n.public.keyhash),Some(0),"infra §4.3: ending the old series cannot remove the newly adopted child's slot");
}

#[test]
fn r06_late_response_must_belong_to_the_records_ceremony() {
    use rhtn_client::{query::*,subject::{SubjectState,SubjectConfig},notice::Silent,store::{ClientStore,OwnSeed},record::take_late_response};
    let (a,b,v)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let backs=vec![vec![[1;32]],vec![[2;32]],vec![[3;32]]];
    let body=presence_record_body(&backs,[&a.public.keyhash,&b.public.keyhash],&[Witness{keyhash:v.public.keyhash,nominated_by:a.public.keyhash,flags:3}],100,101,&[3;32]);
    let rec=Record::parse(&envelope(TYPE_PRESENCE,&body,&[&a,&b,&v])).unwrap();
    let ids=vec![a.public.clone(),b.public.clone(),v.public.clone()];
    assert_eq!(rec.check_signatures(&ids),SigStatus::Verified);
    let mut store=ClientStore::default();
    store.records.insert(rec.txid,rec.bytes.clone());
    store.seeds.insert(rec.txid,OwnSeed{seed:[5;32],counterparty:b.public.keyhash,ceremony_id:[6;32],finalized_at:101});
    let mut subject=SubjectState::new(SubjectConfig::default());
    subject.open_window([7;32]);
    let q=VerificationQuery{subject:a.public.keyhash,querier:b.public.keyhash,ceremony_id:[7;32],profile:vec![1],template_version:1,verifier:v.public.keyhash};
    let consent=subject.consent_to(&a,&q,&Silent).unwrap();
    subject.close_window();
    let response=Response{verifier:v.public.keyhash,subject:a.public.keyhash,query_id:q.query_id(),verdict:Verdict::Match,basis:Some(Basis::PersonalKnowledge),template_version:None,consent,selection_basis:1}.sign(&v);
    rhtn_crypto::verify::response(&ids,&response,false).unwrap();
    let late=LateResponse{record:rec.txid,subject:a.public.keyhash,response}.encode();
    assert!(subject.consented(&[6;32]).is_empty());
    // Pass the ceremony-indexed consent map used by the repaired client dispatch.
    assert!(take_late_response(&mut store,&ids,&late,subject.consented_by_ceremony()).is_err(),"wire §7.4: consenting in ceremony 7 does not authorize evidence for ceremony 6");
}

#[test]
fn r07_directional_scopes_must_be_clipped_to_the_owners_dunbar_org() {
    use rhtn_node::catalog::{ScopeEval,TableScopes};
    use rhtn_archive::catalog::Scope;
    let people=[test_identity("alice"),test_identity("bob"),test_identity("carol"),test_identity("w1")];
    let ids=people.iter().map(|i|i.public.clone()).collect::<Vec<_>>();
    let mut table=Table::new();
    for i in 0..3 {
        let (p,n)=(&people[i],&people[i+1]);
        let raw=envelope(TYPE_ADOPTION,&adoption(n,p,Evidence::Presence([8;32])),&[n,p]);
        table.apply_with(&Record::parse(&raw).unwrap(),&ids,&Missing,None,Evaluation::Deferred).unwrap();
    }
    let eval=TableScopes{table:&table,me:people[1].public.keyhash};
    let (root,leaf)=(&people[0].public.keyhash,&people[3].public.keyhash);
    assert!(eval.in_horizon(leaf),"control: host can answer this requester");
    assert!(!eval.admits(&Scope::Dunbar,root,leaf));
    assert!(!eval.admits(&Scope::Dunbar,leaf,root));
    let escaped=[eval.admits(&Scope::Down(3),root,leaf),eval.admits(&Scope::Up(3),leaf,root)];
    assert_eq!(escaped,[false,false],"wire §6.6: no scope reaches outside the owner's Dunbar Org");
}

#[test]
fn r09_presented_series_chain_must_reject_time_before_its_predecessor() {
    use rhtn_archive::series::SeriesChain;
    let (n,p,former)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let first=adoption(&n,&p,Evidence::Transfer{former:former.public.keyhash,block:transfer_block(&former,&n.public.keyhash,&p.public.keyhash)});
    let a=envelope(TYPE_ADOPTION,&first,&[&n,&p]);
    let predecessor=Record::parse(&a).unwrap();
    let ids=vec![n.public.clone(),p.public.clone(),former.public.clone()];
    let backs=[predecessor.txid];
    let reissue=|time|envelope(TYPE_REISSUE,&reissue_body([&backs,&backs],&n.public.keyhash,&p.public.keyhash,Seqno{series:1,counter:0},2,time),&[&n,&p]);
    assert!(SeriesChain::from_records(&[a.clone(),reissue(101)],&ids).is_ok(),"control: later countersigned successor verifies");
    assert!(SeriesChain::from_records(&[a,reissue(99)],&ids).is_err(),"wire §3.3: a presented reissue cannot predate the adoption at time 100 that both back-pointer lists name");
}

#[test]
fn f04_old_adoption_received_last_must_not_clear_a_new_relationship_slot() {
    use rhtn_node::store::{Decision,KIND_TRANSACTION};
    let (n,p,old)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"));
    let first=adoption(&n,&p,Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) });
    let a=Record::parse(&envelope(TYPE_ADOPTION,&first,&[&n,&p])).unwrap();
    let departed=envelope(TYPE_DEPARTURE,&departure_body(&[a.txid],&n.public.keyhash,&p.public.keyhash,Seqno{series:1,counter:1},101,None),&[&n]);
    let d=Record::parse(&departed).unwrap();
    let bn=[d.txid];let bp=[a.txid];
    let later=adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator{anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno{series:2,counter:0}},timestamp:200,key_material:None,evidence:Evidence::Presence([8;32]),presented_head:None,back:[&bn,&bp] });
    let a2=envelope(TYPE_ADOPTION,&later,&[&n,&p]);
    let mut view=NodeView::new(p.clone(),Locator::root(p.public.keyhash,Seqno{series:3,counter:0}));
    let ids=vec![n.public.clone(),p.public.clone(),old.public];
    for raw in [&departed,&a2,&a.bytes] { assert_eq!(view.take_object(&Capture::default(),&p.public.keyhash,KIND_TRANSACTION,raw,&ids),Decision::Stored); }
    assert!(view.table.patrons(&n.public.keyhash).contains(&p.public.keyhash),"control: new series remains bound");
    assert_eq!(view.slot_of(&n.public.keyhash),Some(0),"infra §4.3: ending the old series cannot remove the newly adopted child's slot");
}

