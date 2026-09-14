//! `rhtn-sim`'s scenarios against daemon processes rather than in-process
//! nodes (`Robot/implementation-plan.md`'s milestone 11).
//!
//! **Nothing here reaches into a view.**  Each daemon is a separate
//! process with its own copy of the state, so a claim is made the way a
//! peer would make it: over a session, or by reading what the process
//! wrote when it stopped.  That is the difference from `sim/tests/live.rs`,
//! which asserts the same behaviour against a `LiveNode` it holds.

use rhtn_archive::record::Record;
use rhtn_archive::tx::*;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::propagation::encode_push;
use rhtn_node::store::KIND_TRANSACTION;
use rhtn_sim::daemons::Daemons;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CAST: [&str; 5] = ["alice", "bob", "carol", "w1", "witness"];

fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

fn kh(n: &str) -> [u8; 32] {
    id(n).public.keyhash
}

fn ids() -> Vec<Identity> {
    CAST.iter().map(|n| id(n).public).collect()
}

fn pins() -> Pins {
    let p = Pins::new();
    for n in CAST {
        p.pin_identity(&id(n).public);
    }
    p
}

fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(id(name)),
        pins: pins(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_secs(5),
        on_reachability: None,
        log: Log::default(),
    }
}

async fn attach_to(name: &str, node: [u8; 32], addr: std::net::SocketAddr) -> Session {
    let ep = tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    match attach(&client_cfg(name), &ep, node, addr, false).await {
        AttachOutcome::Attached(s) => s,
        other => panic!("{name} attaches: {other:?}"),
    }
}

/// The records a scenario pushes, signed as the parties would sign them.
struct Signers {
    archives: BTreeMap<[u8; 32], rhtn_archive::chain::Archive>,
    clock: u64,
}

impl Signers {
    fn new() -> Signers {
        Signers { archives: CAST.iter().map(|n| (kh(n), rhtn_archive::chain::Archive::new(kh(n)))).collect(), clock: 1_800_000_000 }
    }
    fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }
    fn back(&self, n: &str) -> Vec<[u8; 32]> {
        self.archives[&kh(n)].next_back_pointers()
    }
    fn commit(&mut self, ty: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| id(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let rec = Record::parse(&envelope(ty, body, &refs)).expect("well-formed");
        for s in signers {
            self.archives.get_mut(&kh(s)).unwrap().append(rec.clone()).expect("appends");
        }
        rec
    }
    /// A meeting, then `node` adopted under `patron` at `path` below
    /// `anchor`: the pair a scenario pushes together.
    fn adopt(&mut self, node: &str, patron: &str, anchor: &str, path: &[u8]) -> (Record, Record) {
        let t = self.tick();
        let (bn, bp, bw) = (self.back(node), self.back(patron), self.back("witness"));
        let root = rhtn_codec::cose::sha256(format!("m:{node}:{patron}:{t}").as_bytes());
        let w = Witness { keyhash: kh("witness"), nominated_by: kh(patron), flags: 3 };
        let body = presence_record_body(&[bn, bp, bw], [&kh(node), &kh(patron)], &[w], t, t + 600, &root);
        let pop = self.commit(TYPE_PRESENCE, &body, &[node, patron, "witness"]);
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let p = rhtn_node::resolution::Path::from_indices(path);
        let a = Adoption {
            node: kh(node),
            patron: kh(patron),
            locator: Locator { anchor: kh(anchor), path: p.bytes, nibbles: p.nibbles, seqno: Seqno { series: 1, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
        };
        (pop, self.commit(TYPE_ADOPTION, &adoption_body(&a), &[node, patron]))
    }
}

/// Push one object into a daemon on stream 0, the way a peer does.
fn push(s: &Session, bytes: &[u8]) {
    assert!(s.send_control(5, &encode_push(KIND_TRANSACTION, bytes)), "the frame went");
}

/// Wait for a frame carrying `txid` on this session, or give up.
async fn awaits(s: &mut Session, txid: [u8; 32], ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    while let Ok(Some((ft, body))) = tokio::time::timeout_at(deadline, s.frames.recv()).await {
        if ft != 5 {
            continue;
        }
        if let Ok((_, object)) = rhtn_node::propagation::decode_push(&body)
            && Record::parse(&object).is_ok_and(|r| r.txid == txid)
        {
            return true;
        }
    }
    false
}

// acceptance: DMN-17
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transaction_crosses_two_daemon_processes_and_survives_a_restart() {
    let mut set = Daemons::new(env!("CARGO_BIN_EXE_rhtnd"), "flood");
    // P is a root; N runs under it and is configured with P as its patron,
    // so it attaches on the way up
    let p_addr = set.start("alice", &CAST, None);
    let n_addr = set.start("bob", &CAST, Some("alice"));
    assert!(set.running("bob") && set.running("alice"), "both processes are up");

    let mut s = Signers::new();
    // the binding that puts each process in the other's reach: N's own
    // adoption under P.  Its subject is N, so N stores it; its counterparty
    // is P, so P stores it (`wire-format.md` §10.1.1)
    let (n_pop, n_adopt) = s.adopt("bob", "alice", "alice", &[0]);
    // and a second one under N, which only reaches N's store once the
    // first has landed
    let (w_pop, w_adopt) = s.adopt("w1", "bob", "alice", &[0, 3]);

    // an observer attached to N sees what N forwards, which is how a
    // scenario outside both processes learns what N stored
    let mut observer = attach_to("carol", kh("bob"), n_addr).await;
    // and an injector attached to P puts the records in
    let injector = attach_to("w1", kh("alice"), p_addr).await;
    for r in [&n_pop, &n_adopt] {
        push(&injector, &r.bytes);
    }
    assert!(awaits(&mut observer, n_adopt.txid, 5000).await, "N stored its own adoption and forwarded it");

    for r in [&w_pop, &w_adopt] {
        push(&injector, &r.bytes);
    }
    assert!(awaits(&mut observer, w_adopt.txid, 5000).await, "and the adoption under N crossed both processes");

    // **the process wrote down what it accepted.**  A stop is a SIGTERM,
    // and the daemon writes its state back on the way out
    drop(observer);
    set.stop("bob");
    let held = set.get("bob").topology().join("tx");
    let names: Vec<String> = std::fs::read_dir(&held).expect("a topology store").flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    for r in [&n_adopt, &w_adopt] {
        let want: String = r.txid.iter().map(|b| format!("{b:02x}")).collect();
        assert!(names.contains(&want), "the store holds {}", &want[..8]);
    }

    // and it comes back with them: a restart serves at a fresh port and
    // forwards to a new observer what it never received again
    let again = set.restart("bob");
    let mut back = attach_to("carol", kh("bob"), again).await;
    let injector2 = attach_to("w1", kh("alice"), p_addr).await;
    let (c_pop, c_adopt) = s.adopt("carol", "bob", "alice", &[0, 4]);
    for r in [&c_pop, &c_adopt] {
        push(&injector2, &r.bytes);
    }
    assert!(
        awaits(&mut back, c_adopt.txid, 5000).await,
        "the restarted process still holds the binding that puts a subordinate of N in its reach"
    );
    let _ = ids();
}

// acceptance: DMN-18
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_daemon_answers_a_resolution_from_the_topology_it_was_pushed() {
    use rhtn_node::resolution::{Path, REQUEST_RESOLVE, ResolveReply, ResolveRequest};
    let mut set = Daemons::new(env!("CARGO_BIN_EXE_rhtnd"), "resolve");
    let p_addr = set.start("alice", &CAST, None);
    set.start("bob", &CAST, Some("alice"));

    // the topology the daemon will answer from arrives the way any peer's
    // does: over a session, into a process that was told nothing
    let mut s = Signers::new();
    let (n_pop, n_adopt) = s.adopt("bob", "alice", "alice", &[0]);
    let (c_pop, c_adopt) = s.adopt("carol", "bob", "alice", &[0, 2]);
    let mut observer = attach_to("w1", kh("alice"), p_addr).await;
    let injector = attach_to("witness", kh("alice"), p_addr).await;
    for r in [&n_pop, &n_adopt] {
        push(&injector, &r.bytes);
    }
    assert!(awaits(&mut observer, n_adopt.txid, 5000).await, "the first binding landed");
    for r in [&c_pop, &c_adopt] {
        push(&injector, &r.bytes);
    }
    assert!(awaits(&mut observer, c_adopt.txid, 5000).await, "and the second");

    // **the answer comes off a request stream against the process.**  P is
    // the anchor, so it walks the path from itself over the table those two
    // records built
    let req = ResolveRequest { subject: kh("carol"), anchor: kh("alice"), path: Path::from_indices(&[0, 2]).bytes, nibbles: 2, nonce: [8; 16] };
    let bytes = observer.request(REQUEST_RESOLVE, &req.encode()).await.expect("the daemon answers on the stream");
    let reply = ResolveReply::decode(&bytes).expect("a reply");
    assert_eq!(reply.nonce(), [8; 16], "the nonce it was asked with");
    match reply {
        // **which node it names is not asserted here.**  A node's table
        // learns nobody else is infrastructure — nothing in a running node
        // ever marks another one — so the nearest infrastructure ancestor
        // it can see is always itself, and it answers for itself with the
        // residual that identifies the target.  Whether an endpoint record
        // is what marks a publisher infra (`wire-format.md` §7.6: only
        // infra nodes publish) is a question for the author, so this
        // asserts what is unambiguous and leaves that open.
        ResolveReply::Serving { serving, .. } => {
            assert!(!serving.residual.is_empty(), "a residual suffix identifies the party below the node named");
            assert!(!serving.endpoints.is_empty(), "with somewhere to reach it");
        }
        ResolveReply::Referral { referral, .. } => {
            assert!(referral.advances >= 1, "a referral that advances nothing is a loop");
            assert!(!referral.endpoints.is_empty(), "and names where the next hop is");
        }
        other => panic!("a resolution the daemon holds the topology for: {other:?}"),
    }

    // a subject in no record it holds is a failure it can state, not a hang
    let unknown = ResolveRequest { subject: kh("w1"), anchor: kh("alice"), path: Path::from_indices(&[9]).bytes, nibbles: 1, nonce: [9; 16] };
    let bytes = observer.request(REQUEST_RESOLVE, &unknown.encode()).await.expect("answered");
    assert!(matches!(ResolveReply::decode(&bytes).expect("a reply"), ResolveReply::Failure { .. }), "a path it cannot walk is a stated failure");
}
