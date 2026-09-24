//! Independent September 23 review assertions. Failures assert required behavior.
use rhtn_archive::{chain::ArchiveReply, record::Record, tx::*, walk::*};
use rhtn_client::{
    ceremony::{Client, Config},
    payload::*,
};
use rhtn_codec::{cbor::*, encode::*};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_ffi::{device::*, types::*};
use std::{rc::Rc, sync::Arc};

#[derive(Default)]
struct Shell {
    store: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}
impl Proximity for Shell {
    fn supported(&self) -> Vec<Channel> {
        vec![]
    }
    fn run(&self, _: Channel, _: Vec<u8>) -> ChannelOutcome {
        ChannelOutcome::Unavailable
    }
    fn resolution_m(&self, _: Channel) -> Option<u64> {
        None
    }
}
impl Camera for Shell {
    fn capture(&self, _: Ask) -> Vec<u8> {
        vec![7; 64]
    }
}
impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        1_790_000_000_000
    }
    fn wait_ms(&self, _: u64) {}
}
fn fresh(out: &mut [u8]) {
    for b in out {
        *b = rhtn_transport::tls::random_bytes::<1>()[0];
    }
}
impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut b = vec![0; n as usize];
        fresh(&mut b);
        b
    }
}
impl Operator for Shell {
    fn ask(&self, _: String) -> bool {
        true
    }
}
impl Notices for Shell {
    fn told(&self, _: Told) {}
}
impl Storage for Shell {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.store.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.store.lock().unwrap().insert(name, bytes);
        true
    }
}
fn client(name: &str) -> Client {
    let s = Arc::new(Shell::default());
    let p = Platform {
        proximity: s.clone(),
        camera: s.clone(),
        clock: s.clone(),
        random: s.clone(),
        operator: s.clone(),
        notices: s.clone(),
        storage: s,
    };
    Client::new(
        test_identity(name),
        vec![test_identity("alice").public, test_identity("bob").public],
        Config::default(),
        p.device(Rc::new(rhtn_client::device::NoDirectPath)),
    )
}

// Reuse only the repository's signed topology and loopback fixtures. The
// post-failover assertion below is independent of its status-only test.
#[path = "../../sim/tests/common/mod.rs"]
mod network_fixture;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r08_failover_must_update_the_kernel_serving_identity() {
    use network_fixture::*;
    use rhtn_ffi::{client::Participant, net::Status};
    use rhtn_node::{
        resolution::{AnchorTable, Ingestion},
        runtime::LiveNode,
    };
    use rhtn_transport::session::NetworkPoint;
    struct Quiet;
    impl rhtn_node::Adjacency for Quiet {
        fn peers(&self) -> Vec<[u8; 32]> {
            vec![]
        }
        fn send(&self, _: &[u8; 32], _: u64, _: &[u8]) {}
        fn request(&self, _: &[u8; 32], _: u64, _: &[u8]) -> bool {
            false
        }
    }
    let mut sg = Signers::new();
    let an = sg.adopt("bob", "alice2", "alice2", &[0], 1);
    let asib = sg.adopt("carol", "alice2", "alice2", &[1], 2);
    let ac = sg.adopt("alice", "bob", "alice2", &[0, 0], 3);
    let records = [&an, &asib, &ac];
    let infra = ["alice2", "bob", "carol"];
    let anchors = || AnchorTable::new(0, Ingestion::UnverifiedGossip);
    let sibling = LiveNode::start(
        node_cfg("carol", 1),
        view_of(
            "carol",
            table_of("carol", &sg, &records, &infra),
            "alice2",
            &[1],
            sg.clock,
        ),
        ids(),
        anchors(),
    );
    let mut view = view_of(
        "bob",
        table_of("bob", &sg, &records, &infra),
        "alice2",
        &[0],
        sg.clock,
    );
    view.set_slot(0, Some(kh("alice")), sg.clock);
    view.attached.insert(kh("alice"));
    let endpoint = rhtn_node::resolution::endpoint_record(
        &id("carol"),
        &[NetworkPoint::from_socket(sibling.addr).unwrap()],
        Seqno {
            series: 1,
            counter: 1,
        },
    );
    view.take_object(
        &Quiet,
        &kh("carol"),
        rhtn_node::store::KIND_ENDPOINT_RECORD,
        &endpoint,
        &ids(),
    );
    let mut cfg = node_cfg("bob", 1);
    cfg.filter = Some(drop_heartbeats());
    let primary = LiveNode::start(cfg, view, ids(), anchors());
    let addr = primary.addr.to_string();
    let shell = Arc::new(Shell::default());
    let s = shell.clone();
    let p = tokio::task::spawn_blocking(move || {
        let platform = Arc::new(Platform {
            proximity: s.clone(),
            camera: s.clone(),
            clock: s.clone(),
            random: s.clone(),
            operator: s.clone(),
            notices: s.clone(),
            storage: s,
        });
        let mut seeds = rhtn_codec::cose::sha256(b"rhtn-test-vectors:alice:ed25519-seed").to_vec();
        seeds.extend_from_slice(&rhtn_codec::cose::sha256(
            b"rhtn-test-vectors:alice:ml-dsa-65-seed",
        ));
        let p = Participant::start(
            seeds,
            ids().iter().map(|x| x.key_material()).collect(),
            platform,
        )
        .unwrap();
        p.attach(kh("bob").to_vec(), vec![addr], vec![]).unwrap();
        p
    })
    .await
    .unwrap();
    let p = Arc::new(p);
    assert!(
        until(
            15000,
            || matches!(p.status(),Status::Attached{serving,..} if serving==kh("carol"))
        )
        .await,
        "control: live failover reached Carol"
    );
    let q = p.clone();
    tokio::task::spawn_blocking(move || q.save())
        .await
        .unwrap()
        .unwrap();
    let saved = shell
        .store
        .lock()
        .unwrap()
        .values()
        .find(|bytes| matches!(parse_all(bytes),Ok(Item::Array(ref a)) if a.len()==7))
        .unwrap()
        .clone();
    let fields = array_item_ranges(&saved, 0).unwrap();
    let field = &saved[fields[2].clone()];
    let Item::Bytes(ref r) = parse_all(field).unwrap() else {
        panic!("payload")
    };
    let payload = PayloadState::decode(Default::default(), &field[r.clone()]).unwrap();
    let observed = payload.serving;
    let q = p.clone();
    tokio::task::spawn_blocking(move || q.detach())
        .await
        .unwrap()
        .unwrap();
    drop(p);
    drop(primary);
    drop(sibling);
    assert_eq!(
        observed,
        Some(kh("carol")),
        "SES-010/CAT-017: transport moved to Carol while the kernel still attributes serving operations to Bob"
    );
}

#[test]
fn control_durable_state_round_trips_for_its_owner() {
    let a = client("alice");
    let bytes = a.durable();
    let mut b = client("alice");
    assert!(b.restore_durable(&bytes).is_ok());
    assert_eq!(b.durable(), bytes);
}

#[test]
fn r01_failed_durable_restore_must_leave_all_state_unchanged() {
    let mut a = client("alice");
    let before = a.durable();
    let donor = client("alice").durable();
    let ranges = array_item_ranges(&donor, 0).unwrap();
    let mut corrupt = vec![];
    emit_array_head(&mut corrupt, 7);
    for (i, r) in ranges.into_iter().enumerate() {
        if i == 5 {
            emit_bstr(&mut corrupt, &[0xff]);
        } else {
            corrupt.extend_from_slice(&donor[r]);
        }
    }
    assert!(
        a.restore_durable(&corrupt).is_err(),
        "control: malformed delegation store refused"
    );
    assert!(
        a.durable() == before,
        "APP-008/OPS-009: refusal replaced the payload/store before validating delegations"
    );
}

#[test]
fn r02_durable_state_of_another_identity_must_be_refused() {
    let a = client("alice");
    let mut b = client("bob");
    let before = b.durable();
    let result = b.restore_durable(&a.durable());
    assert!(
        result.is_err(),
        "APP-008: Bob accepted Alice's durable state: {result:?}"
    );
    assert_eq!(b.durable(), before);
}

#[test]
fn r03_pagination_must_preserve_archive_edge_chronology() {
    let a = test_identity("alice");
    let b = test_identity("bob");
    let ids = vec![a.public.clone(), b.public.clone()];
    let make = |back, time, counter| {
        Record::parse(&envelope(
            TYPE_DEPARTURE,
            &departure_body(
                &[back],
                &a.public.keyhash,
                &b.public.keyhash,
                Seqno { series: 1, counter },
                time,
                None,
            ),
            &[&a],
        ))
        .unwrap()
    };
    let parent = make(rhtn_archive::genesis(&a.public.keyhash), 200, 1);
    let child = make(parent.txid, 100, 2);
    let together = ArchiveReply {
        nonce: [0; 16],
        records: vec![child.bytes.clone(), parent.bytes.clone()],
        more: false,
        frontier: vec![],
    };
    assert!(
        matches!(
            verify_batch(&a.public.keyhash, &[child.txid], &together, &ids).end,
            BatchEnd::Mismatch { .. }
        ),
        "control: one page rejects reversed chronology"
    );
    let mut pages = 0;
    let got = fetch_chain(&a.public.keyhash, Some(child.txid), 1, &ids, |q| {
        pages += 1;
        assert!(pages <= 2);
        if q.frontier == vec![child.txid] {
            ArchiveReply {
                nonce: q.nonce,
                records: vec![child.bytes.clone()],
                more: true,
                frontier: vec![parent.txid],
            }
        } else {
            assert_eq!(q.frontier, vec![parent.txid]);
            ArchiveReply {
                nonce: q.nonce,
                records: vec![parent.bytes.clone()],
                more: false,
                frontier: vec![],
            }
        }
    });
    assert_eq!(pages, 2);
    assert!(
        !got.verified_complete,
        "ARC-003/ARC-010: the same invalid edge was verified complete across pages: {got:?}"
    );
}

fn open_message(relabel: bool) -> (Result<Vec<u8>, PayloadError>, bool) {
    let a = test_identity("alice");
    let b = test_identity("bob");
    let ids = vec![a.public.clone(), b.public.clone()];
    let ad = [1; 32];
    let bd = [2; 32];
    let fake = [99; 32];
    let mut ak = PayloadKeys::generate(&mut fresh, 100);
    let mut bk = PayloadKeys::generate(&mut fresh, 100);
    let ab = read_bundle(&ids, &ak.bundle(&a, &ad, 100)).unwrap();
    let bb = read_bundle(&ids, &bk.bundle(&b, &bd, 100)).unwrap();
    let mut sender = Sessions::default();
    let mut receiver = Sessions::default();
    receiver.prefetch(ab);
    let mut wire = sender
        .open(&ak, &ad, b.public.keyhash, &bb, None, &mut fresh, b"hello")
        .unwrap();
    if relabel {
        let ranges = array_item_ranges(&wire, 0).unwrap();
        let mut edited = vec![];
        emit_array_head(&mut edited, 3);
        for (i, r) in ranges.into_iter().enumerate() {
            if i == 1 {
                emit_bstr(&mut edited, &fake);
            } else {
                edited.extend_from_slice(&wire[r]);
            }
        }
        wire = edited;
    }
    let result = receiver.receive(&mut bk, a.public.keyhash, &wire, &mut fresh);
    (result, receiver.has_session_with(&a.public.keyhash, &fake))
}
#[test]
fn control_payload_opens_under_the_signed_sending_device() {
    assert_eq!(open_message(false).0.unwrap(), b"hello");
}
#[test]
fn r04_initial_payload_must_authenticate_the_sending_device() {
    let (result, installed_fake) = open_message(true);
    assert!(
        result.is_err() && !installed_fake,
        "MAIL-012/PAY-004: unsigned device rewrite accepted: {result:?}; fake device session={installed_fake}"
    );
}

#[test]
fn r05_completed_catalog_refresh_must_remove_withdrawn_entries() {
    use rhtn_archive::catalog::*;
    use rhtn_client::catalog::*;
    let a = test_identity("alice");
    let host = test_identity("bob").public.keyhash;
    let ids = vec![a.public.clone()];
    let entry = CatalogEntry::build(
        &a,
        &EntryFields {
            resource: [7; 32],
            service_type: "rhtn-forum".into(),
            instance: "review".into(),
            endpoint: b"e".to_vec(),
            connect_scope: None,
            metadata: None,
            data_practice: None,
        },
    );
    let mut view = View::default();
    let mut first = Sweep::default();
    let q = first.query(None, [1; 16]);
    assert_eq!(
        first.take(
            &ids,
            view.portion(host),
            &CatalogReply {
                nonce: q.nonce,
                entries: vec![entry],
                continuation: None
            },
            111
        ),
        Step::Done
    );
    assert_eq!(view.entries().len(), 1);
    let mut refresh = Sweep::default();
    let q = refresh.query(None, [2; 16]);
    assert_eq!(
        refresh.take(
            &ids,
            view.portion(host),
            &CatalogReply {
                nonce: q.nonce,
                entries: vec![],
                continuation: None
            },
            111
        ),
        Step::Done
    );
    assert!(
        view.entries().is_empty(),
        "CAT-016/CAT-018: successful empty refresh retained the withdrawn entry"
    );
}

#[test]
fn r06_a_valid_presented_presence_must_be_walkable_in_an_archive_reply() {
    use rhtn_client::record::{disclosure_root, disclosures, present};
    let a = test_identity("alice");
    let b = test_identity("bob");
    let ids = vec![a.public.clone(), b.public.clone()];
    let set = disclosures(std::array::from_fn(|_| vec![0]), [[4; 16]; 7]);
    let ga = [rhtn_archive::genesis(&a.public.keyhash)];
    let gb = [rhtn_archive::genesis(&b.public.keyhash)];
    let env = envelope(
        TYPE_PRESENCE,
        &formation_body(
            [&ga, &gb],
            [&a.public.keyhash, &b.public.keyhash],
            100,
            101,
            &disclosure_root(&set),
        ),
        &[&a, &b],
    );
    let rec = Record::parse(&env).unwrap();
    let presented = present(&env, &set, &[]);
    assert!(
        rhtn_crypto::verify::presentation(&ids, &presented).is_ok(),
        "control: valid hybrid signatures and seven withheld commitments"
    );
    let reply = ArchiveReply {
        nonce: [1; 16],
        records: vec![presented],
        more: false,
        frontier: vec![],
    };
    let decoded = ArchiveReply::decode(&reply.encode()).unwrap();
    let got = verify_batch(&a.public.keyhash, &[rec.txid], &decoded, &ids);
    assert_eq!(
        got.end,
        BatchEnd::Genesis,
        "SCH-018/ARC-016: valid presence accepted by reply decoder, refused by archive verifier"
    );
}

fn connected_clients() -> (Client, Client) {
    let mut a = client("alice");
    let mut b = client("bob");
    let ids = vec![test_identity("alice").public, test_identity("bob").public];
    let ab = read_bundle(
        &ids,
        &a.payload
            .keys
            .bundle(&test_identity("alice"), &a.payload.device, 100),
    )
    .unwrap();
    let bb = read_bundle(
        &ids,
        &b.payload
            .keys
            .bundle(&test_identity("bob"), &b.payload.device, 100),
    )
    .unwrap();
    a.payload.sessions.prefetch(bb.clone());
    b.payload.sessions.prefetch(ab);
    let wire = a
        .payload
        .sessions
        .open(
            &a.payload.keys,
            &a.payload.device,
            b.keyhash(),
            &bb,
            None,
            &mut fresh,
            &wrap(rhtn_client::payload::KIND_APPLICATION, b"connected"),
        )
        .unwrap();
    assert!(matches!(
        b.receive_payload(a.keyhash(), &wire).unwrap(),
        rhtn_client::ceremony::Dispatched::Application(_)
    ));
    (a, b)
}
fn deliver(from: &Client, to: &mut Client, msgs: Vec<rhtn_client::ceremony::Msg>) {
    use rhtn_client::ceremony::Msg;
    for msg in msgs {
        match msg {
            Msg::Payload { bytes, .. } | Msg::Relay { bytes, .. } => {
                to.receive_payload(from.keyhash(), &bytes).unwrap();
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
#[test]
fn r07_client_archive_fetch_must_continue_when_more_is_true() {
    let (mut asker, mut subject) = connected_clients();
    let b = test_identity("bob");
    let a = test_identity("alice");
    for i in 1..=2 {
        let rec = Record::parse(&envelope(
            TYPE_DEPARTURE,
            &departure_body(
                &subject.archive.next_back_pointers(),
                &b.public.keyhash,
                &a.public.keyhash,
                Seqno {
                    series: 1,
                    counter: i,
                },
                100 + i as u64,
                None,
            ),
            &[&b],
        ))
        .unwrap();
        subject.archive.append(rec).unwrap();
    }
    let head = subject.archive.next_back_pointers()[0];
    let msgs = asker
        .fetch_archive(subject.keyhash(), Some(head), 1)
        .unwrap();
    deliver(&asker, &mut subject, msgs);
    let reply = subject.outbox();
    assert_eq!(reply.len(), 1);
    deliver(&subject, &mut asker, reply);
    assert_eq!(
        asker.store.records.len(),
        1,
        "control: page one arrived and was verified"
    );
    assert!(
        !asker.outbox().is_empty(),
        "ARC-010/ARC-016: reply says more, but client discards frontier and posts no continuation"
    );
}

#[test]
fn r07_client_archive_fetch_must_keep_every_valid_merge_branch() {
    let (mut asker, mut subject) = connected_clients();
    let b = test_identity("bob");
    let a = test_identity("alice");
    for i in 1..=2 {
        let rec = Record::parse(&envelope(
            TYPE_DEPARTURE,
            &departure_body(
                &[rhtn_archive::genesis(&b.public.keyhash)],
                &b.public.keyhash,
                &a.public.keyhash,
                Seqno {
                    series: 1,
                    counter: i,
                },
                100 + i as u64,
                None,
            ),
            &[&b],
        ))
        .unwrap();
        subject.archive.append(rec).unwrap();
    }
    assert_eq!(subject.archive.next_back_pointers().len(), 2);
    let merged = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &subject.archive.next_back_pointers(),
            &b.public.keyhash,
            &a.public.keyhash,
            Seqno {
                series: 1,
                counter: 3,
            },
            103,
            None,
        ),
        &[&b],
    ))
    .unwrap();
    let head = merged.txid;
    subject.archive.append(merged).unwrap();
    let reply = subject.archive.serve(&rhtn_archive::chain::ArchiveRequest {
        subject: b.public.keyhash,
        frontier: vec![head],
        max_records: 16,
        stop_before: None,
        nonce: [1; 16],
    });
    assert_eq!(
        verify_batch(
            &b.public.keyhash,
            &[head],
            &reply,
            &vec![a.public.clone(), b.public.clone()]
        )
        .end,
        BatchEnd::Genesis,
        "control: the three-record DAG is complete"
    );
    let request = asker
        .fetch_archive(subject.keyhash(), Some(head), 16)
        .unwrap();
    deliver(&asker, &mut subject, request);
    let reply = subject.outbox();
    deliver(&subject, &mut asker, reply);
    assert_eq!(
        asker.store.records.len(),
        3,
        "ARC-002/ARC-016: linear client walk rejected the second valid branch"
    );
}

#[test]
fn r02_backup_install_must_not_accept_another_identitys_seeds() {
    use rhtn_client::backup::{self, Contents, Cost, Wrap};
    let mut b = client("bob");
    let contents = Contents {
        seeds: Some([[71; 32], [72; 32]]),
        provider: Some(b"foreign-provider-secret".to_vec()),
        ..Default::default()
    };
    let donor = rhtn_crypto::SigningIdentity::from_seeds(&[71; 32], &[72; 32]);
    assert_ne!(b.keyhash(), donor.public.keyhash);
    let blob = backup::export(
        &contents,
        &Wrap::passphrase(Cost {
            m_kib: 32,
            passes: 1,
            lanes: 1,
        }),
        b"password",
    )
    .unwrap();
    let (opened, _) = b.import(&blob, b"password").unwrap();
    assert_eq!(opened.seeds, contents.seeds);
    let result = b.install(opened);
    assert!(
        result.is_err(),
        "OPS-004: backup seeds discarded and foreign provider/store installed under Bob: {result:?}"
    );
}

#[test]
fn r09_topology_persistence_must_not_keep_foreign_transaction_history() {
    use network_fixture::*;
    use rhtn_node::store::{Decision, KIND_TRANSACTION};
    struct Quiet;
    impl rhtn_node::Adjacency for Quiet {
        fn peers(&self) -> Vec<[u8; 32]> {
            vec![]
        }
        fn send(&self, _: &[u8; 32], _: u64, _: &[u8]) {}
        fn request(&self, _: &[u8; 32], _: u64, _: &[u8]) -> bool {
            false
        }
    }
    let mut sg = Signers::new();
    let parent = sg.adopt("bob", "alice", "alice", &[0], 1);
    let foreign = sg.adopt("carol", "bob", "alice", &[0, 0], 2);
    let mut view = view_of(
        "alice",
        table_of("alice", &sg, &[&parent, &foreign], &["alice", "bob"]),
        "alice",
        &[],
        sg.clock,
    );
    assert!(
        !foreign.signers.contains(&kh("alice")),
        "control: observer did not sign this act"
    );
    assert_eq!(
        view.take_object(&Quiet, &kh("bob"), KIND_TRANSACTION, &foreign.bytes, &ids()),
        Decision::Stored
    );
    let c = id("carol");
    let departure = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &[foreign.txid],
            &kh("carol"),
            &kh("bob"),
            Seqno {
                series: 2,
                counter: 1,
            },
            sg.clock + 1,
            None,
        ),
        &[&c],
    ))
    .unwrap();
    assert_eq!(
        view.take_object(
            &Quiet,
            &kh("bob"),
            KIND_TRANSACTION,
            &departure.bytes,
            &ids()
        ),
        Decision::Stored
    );
    let dir = std::env::temp_dir().join(format!("fueros-review-history-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    view.store.save(&dir).unwrap();
    fn every_file(dir: &std::path::Path, out: &mut Vec<Vec<u8>>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    every_file(&path, out);
                } else if let Ok(bytes) = std::fs::read(path) {
                    out.push(bytes);
                }
            }
        }
    }
    let mut files = Vec::new();
    every_file(&dir, &mut files);
    let retained =
        files.iter().any(|b| b == &foreign.bytes) || files.iter().any(|b| b == &departure.bytes);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        !retained,
        "I 4.3: persisted full third-party adoption/departure history instead of the seen fact and current topology"
    );
}

#[test]
fn r02_seedless_backup_must_still_be_bound_to_its_identity() {
    use rhtn_client::backup::{self, Contents, Cost, Wrap};
    let mut bob = client("bob");
    // A backup made on a delegated device has no identity seeds.  Its
    // provider credential and evidence store are still identity state, so
    // omitting the seeds cannot make the envelope transferable to any empty
    // client that knows its passphrase.
    let foreign = Contents {
        seeds: None,
        provider: Some(b"alice-provider-secret".to_vec()),
        ..Default::default()
    };
    let blob = backup::export(
        &foreign,
        &Wrap::passphrase(Cost {
            m_kib: 32,
            passes: 1,
            lanes: 1,
        }),
        b"password",
    )
    .unwrap();
    let (opened, _) = bob.import(&blob, b"password").unwrap();
    let result = bob.install(opened);
    assert!(
        result.is_err(),
        "OPS-004: a seedless backup names no owner and installed foreign provider state into Bob: {result:?}"
    );
    assert!(bob.provider_credential.is_none());
}

#[test]
fn n01_current_client_must_open_the_previous_durable_format() {
    // HEAD wrote field 6 as the optional provider credential.  The current
    // worktree replaced it in place with [owner, provider].  An upgrade must
    // retain the authoritative state written by the immediately preceding
    // implementation rather than refusing startup over it.
    let before = client("alice");
    let current = before.durable();
    let fields = array_item_ranges(&current, 0).unwrap();
    let mut previous = Vec::new();
    emit_array_head(&mut previous, 7);
    for range in fields.into_iter().take(6) {
        previous.extend_from_slice(&current[range]);
    }
    emit_array_head(&mut previous, 0); // the old optional-provider None
    let mut after = client("alice");
    let result = after.restore_durable(&previous);
    assert!(
        result.is_ok(),
        "APP-008/OPS-009: the new reader refuses durable state written by the preceding implementation: {result:?}"
    );
}

#[test]
fn n02_archive_reply_rejection_must_not_commit_a_verified_prefix() {
    use rhtn_client::ceremony::{Dispatched, Msg};
    let (mut asker, mut subject) = connected_clients();
    let a = test_identity("alice");
    let b = test_identity("bob");
    let rec = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &[rhtn_archive::genesis(&b.public.keyhash)],
            &b.public.keyhash,
            &a.public.keyhash,
            Seqno {
                series: 1,
                counter: 1,
            },
            100,
            None,
        ),
        &[&b],
    ))
    .unwrap();
    let unrelated = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &[rhtn_archive::genesis(&a.public.keyhash)],
            &a.public.keyhash,
            &b.public.keyhash,
            Seqno {
                series: 1,
                counter: 1,
            },
            100,
            None,
        ),
        &[&a],
    ))
    .unwrap();

    let request = asker
        .fetch_archive(subject.keyhash(), Some(rec.txid), 16)
        .unwrap();
    let wire = match request.as_slice() {
        [Msg::Payload { bytes, .. }] | [Msg::Relay { bytes, .. }] => bytes.clone(),
        other => panic!("one encrypted archive request, got {other:?}"),
    };
    // Read the request through a copy of the recipient's ratchet, leaving the
    // real subject able to send the deliberately malformed reply.
    let mut probe = PayloadState::decode(subject.payload.cfg.clone(), &subject.payload.encode())
        .expect("payload state clones");
    let plain = probe
        .sessions
        .receive(&mut probe.keys, asker.keyhash(), &wire, &mut fresh)
        .unwrap();
    let (kind, inner) = rhtn_client::payload::unwrap(&plain).unwrap();
    assert_eq!(kind, rhtn_client::payload::KIND_ARCHIVE_REQUEST);
    let asked = rhtn_archive::chain::ArchiveRequest::decode(&inner).unwrap();

    let reply = ArchiveReply {
        nonce: asked.nonce,
        records: vec![rec.bytes.clone(), unrelated.bytes],
        more: false,
        frontier: vec![],
    };
    let forged = subject
        .send_payload(
            asker.keyhash(),
            rhtn_client::payload::KIND_ARCHIVE_REPLY,
            &reply.encode(),
        )
        .unwrap();
    for msg in forged {
        let bytes = match msg {
            Msg::Payload { bytes, .. } | Msg::Relay { bytes, .. } => bytes,
            other => panic!("unexpected reply path: {other:?}"),
        };
        assert!(matches!(
            asker.receive_payload(subject.keyhash(), &bytes),
            Ok(Dispatched::Fetched(Err(_)))
        ));
    }
    assert!(
        !asker.store.records.contains_key(&rec.txid),
        "ARC-016 and the common rejection rule: a malformed reply committed its valid prefix"
    );
}

#[test]
fn n03_upgrade_must_convert_a_legacy_transaction_to_its_seen_fact() {
    let carol = test_identity("carol");
    let bob = test_identity("bob");
    let foreign = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &[rhtn_archive::genesis(&carol.public.keyhash)],
            &carol.public.keyhash,
            &bob.public.keyhash,
            Seqno {
                series: 1,
                counter: 1,
            },
            100,
            None,
        ),
        &[&carol],
    ))
    .unwrap();
    let dir =
        std::env::temp_dir().join(format!("fueros-review-upgrade-seen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tx = dir.join("tx");
    std::fs::create_dir_all(&tx).unwrap();
    std::fs::write(tx.join("legacy-foreign"), &foreign.bytes).unwrap();
    let store = rhtn_node::store::TopologyStore::load(&dir).unwrap();
    let remembered = store.holds_txid(&foreign.txid);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        remembered,
        "TOP-035/OPS-016: upgrade ignored the preceding store instead of retaining each transaction's seen fact"
    );
}

#[test]
fn n04_upgrade_must_not_delete_a_legacy_own_act_while_migrating_the_store() {
    let alice = test_identity("alice");
    let bob = test_identity("bob");
    let own = Record::parse(&envelope(
        TYPE_DEPARTURE,
        &departure_body(
            &[rhtn_archive::genesis(&alice.public.keyhash)],
            &alice.public.keyhash,
            &bob.public.keyhash,
            Seqno {
                series: 1,
                counter: 1,
            },
            100,
            None,
        ),
        &[&alice],
    ))
    .unwrap();
    let dir =
        std::env::temp_dir().join(format!("fueros-review-upgrade-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tx = dir.join("tx");
    std::fs::create_dir_all(&tx).unwrap();
    std::fs::write(tx.join("legacy-own"), &own.bytes).unwrap();
    let store = rhtn_node::store::TopologyStore::load(&dir).unwrap();
    store.save(&dir).unwrap();
    let replayed = store.objects();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        replayed
            .iter()
            .any(|(kind, bytes)| *kind == rhtn_node::store::KIND_TRANSACTION && bytes == &own.bytes),
        "TOP-035/OPS-016: upgrade deleted a signed act of this node instead of migrating it to the retained own-act set"
    );
}

#[test]
fn control_start_from_backup_restores_the_identity_and_writes_current_state() {
    use rhtn_client::backup::Cost;
    use rhtn_ffi::client::{Participant, STATE};
    let source = client("alice");
    let ed = rhtn_codec::cose::sha256(b"rhtn-test-vectors:alice:ed25519-seed");
    let pq = rhtn_codec::cose::sha256(b"rhtn-test-vectors:alice:ml-dsa-65-seed");
    let blob = source
        .export(
            Some([ed, pq]),
            Cost {
                m_kib: 32,
                passes: 1,
                lanes: 1,
            },
            b"password",
        )
        .unwrap();
    let shell = Arc::new(Shell::default());
    let platform = Arc::new(Platform {
        proximity: shell.clone(),
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell.clone(),
        storage: shell.clone(),
    });
    let restored = Participant::start_from_backup(
        blob,
        b"password".to_vec(),
        vec![
            test_identity("alice").public.key_material(),
            test_identity("bob").public.key_material(),
        ],
        platform,
    )
    .unwrap();
    assert_eq!(
        restored.me(),
        test_identity("alice").public.keyhash.to_vec()
    );
    assert!(
        shell.store.lock().unwrap().contains_key(STATE),
        "a successful replacement-device restore writes current durable state"
    );
}
