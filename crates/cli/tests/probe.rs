//! `rhtn probe` against a running node (`wire-format.md` §9.2's read-only
//! class): resolve, archive fetch and catalog query over a real session,
//! and nothing that changes state.

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::resolution::{AnchorTable, Ingestion, Path as NodePath};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::{Log, NodeConfig};
use rhtn_transport::tls::Pins;
use std::process::Command;
use std::sync::Arc;

fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn ids() -> Vec<rhtn_crypto::Identity> {
    ["alice", "bob", "carol", "witness"]
        .iter()
        .map(|n| test_identity(n).public)
        .collect()
}

fn pins() -> Pins {
    let p = Pins::new();
    for id in ids() {
        p.pin_identity(&id);
    }
    p
}

/// An identity file the command line can read, holding the seeds the
/// testkit derives.
fn identity_file(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let mut b =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    let p = dir.join(format!("{name}.key"));
    std::fs::write(&p, b).unwrap();
    p
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rhtn"))
        .args(args)
        .output()
        .expect("rhtn runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

// acceptance: DMN-06
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_probe_resolves_fetches_an_archive_and_queries_a_catalog_over_a_real_session() {
    let dir = std::env::temp_dir().join(format!("rhtn-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // bob serves, with carol adopted beneath it and its own archive holding
    // that adoption
    let pop = {
        let g = |n: &str| vec![rhtn_archive::genesis(&kh(n))];
        let root = rhtn_codec::cose::sha256(b"a meeting");
        let w = Witness {
            keyhash: kh("witness"),
            nominated_by: kh("bob"),
            flags: 3,
        };
        let body = presence_record_body(
            &[g("bob"), g("carol"), g("witness")],
            [&kh("bob"), &kh("carol")],
            &[w],
            1_800_000_000,
            1_800_000_600,
            &root,
        );
        envelope(
            TYPE_PRESENCE,
            &body,
            &[
                &test_identity("bob"),
                &test_identity("carol"),
                &test_identity("witness"),
            ],
        )
    };
    let pop_id = Record::parse(&pop).unwrap().txid;
    let path = NodePath::from_indices(&[0]);
    let adoption = {
        let a = Adoption {
            node: kh("carol"),
            patron: kh("bob"),
            locator: Locator {
                anchor: kh("bob"),
                path: path.bytes.clone(),
                nibbles: path.nibbles,
                seqno: Seqno {
                    series: 1,
                    counter: 0,
                },
            },
            timestamp: 1_800_003_600,
            key_material: None,
            evidence: Evidence::Presence(pop_id),
            presented_head: None,
            back: [&[pop_id], &[pop_id]],
        };
        envelope(
            TYPE_ADOPTION,
            &adoption_body(&a),
            &[&test_identity("carol"), &test_identity("bob")],
        )
    };
    let mut table = Table::with_me(kh("bob"));
    table.mark_infra(kh("bob"));
    let store = std::collections::BTreeMap::from([(pop_id, pop.clone())]);
    table
        .apply(&Record::parse(&adoption).unwrap(), &ids(), &store, None)
        .expect("applies");
    let mut view = NodeView::new(
        Arc::new(test_identity("bob")),
        Locator {
            anchor: kh("bob"),
            path: Vec::new(),
            nibbles: 0,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
        },
    );
    view.table = table;
    view.set_slot(0, Some(kh("carol")), 1_800_003_600);
    let mut archive = Archive::new(kh("bob"));
    archive.append(Record::parse(&pop).unwrap()).unwrap();
    archive.append(Record::parse(&adoption).unwrap()).unwrap();
    view.archive = archive;
    let mut cfg = NodeConfig::defaults(Arc::new(test_identity("bob")), pins(), 30);
    cfg.log = Log::recording();
    let node = LiveNode::start(
        cfg,
        view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    );
    let addr = node.addr.to_string();

    // carol is adopted beneath bob, so it is inside the horizon bob
    // answers a catalog query within (`wire-format.md` §6.4)
    let me = identity_file(&dir, "carol");
    let me = me.to_string_lossy().to_string();
    let target = hex(&kh("bob"));
    let peer = format!(
        "{target}:{}",
        hex(&test_identity("bob").public.key_material())
    );
    let base = ["probe", "--peer", &peer, &me, &target, &addr];

    // resolve: bob is authoritative for carol at index 0 beneath it
    let (ok, out, err) = run(&[&base[..], &["resolve", &hex(&kh("carol")), &target, "0"]].concat());
    assert!(ok, "resolve: {err}");
    assert!(
        out.contains("ServingInfra") || out.contains("Referral") || out.contains("Failure"),
        "an answer of some shape: {out}"
    );
    // archive fetch: the two records bob holds, head first
    let (ok, out, err) = run(&[&base[..], &["archive", &target]].concat());
    assert!(ok, "archive: {err}");
    assert!(
        out.contains("records   2"),
        "the two records bob signed: {out}"
    );
    assert!(
        out.contains(&hex(&Record::parse(&adoption).unwrap().txid)),
        "naming the adoption: {out}"
    );
    // catalog: an empty catalog answers, and answering is the point
    let (ok, out, err) = run(&[&base[..], &["catalog"]].concat());
    assert!(ok, "catalog: {err}");
    assert!(
        out.contains("entries   0"),
        "an empty catalog is an answer: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_command_line_refuses_what_it_will_not_send() {
    // the ask is one of three, and a prekey fetch is not among them
    let (ok, _, err) = run(&["probe", "k", "k", "1.2.3.4:1", "prekey", "k"]);
    assert!(!ok && err.contains("resolve, archive or catalog"), "{err}");
    // and an unknown command says what there is
    let (ok, _, err) = run(&["send"]);
    assert!(
        !ok && err.contains("Nothing here sends a request that changes state"),
        "{err}"
    );
}

// acceptance: DMN-09
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_archive_batch_that_is_not_a_chain_is_refused_rather_than_reported() {
    use rhtn_archive::chain::{ArchiveReply, ArchiveRequest};
    use rhtn_codec::schema::Family;
    use rhtn_transport::session::Node;
    use std::sync::Mutex;
    // two records bob signed that both point at genesis: each is valid on
    // its own and the pair is not a chain
    let g = vec![rhtn_archive::genesis(&kh("bob"))];
    let loose = |code: u64| {
        let body = disavowal_body(
            &g,
            &kh("bob"),
            &kh("carol"),
            1_800_000_000 + code,
            Some(code),
        );
        envelope(TYPE_DISAVOWAL, &body, &[&test_identity("bob")])
    };
    let (d1, d2) = (loose(1), loose(2));
    // a node that serves whichever batch it is told to
    let disconnected: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let (chain, broken) = (vec![d1.clone()], vec![d1.clone(), d2.clone()]);
    let flag = disconnected.clone();
    let mut cfg = NodeConfig::defaults(Arc::new(test_identity("bob")), pins(), 30);
    cfg.log = Log::recording();
    cfg.on_request = Some(Arc::new(move |_peer, family, body| {
        let (flag, chain, broken) = (flag.clone(), chain.clone(), broken.clone());
        Box::pin(async move {
            if family != Family::ArchiveRequest {
                return None;
            }
            let req = ArchiveRequest::decode(&body).ok()?;
            let records = if *flag.lock().unwrap() { broken } else { chain };
            Some(
                ArchiveReply {
                    nonce: req.nonce,
                    records,
                    more: false,
                    frontier: Vec::new(),
                }
                .encode(),
            )
        })
    }));
    let ep = rhtn_transport::tls::server_endpoint(&cfg.identity, "127.0.0.1:0".parse().unwrap())
        .unwrap();
    let addr = ep.local_addr().unwrap();
    tokio::spawn(Node::new(cfg).serve(ep));

    let dir = std::env::temp_dir().join(format!("rhtn-chain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let me = identity_file(&dir, "carol").to_string_lossy().to_string();
    let target = hex(&kh("bob"));
    let peer = format!(
        "{target}:{}",
        hex(&test_identity("bob").public.key_material())
    );
    let at = addr.to_string();
    let base = ["probe", "--peer", &peer, &me, &target, &at];

    // one record is trivially a chain, and the reply says what was not checked
    let (ok, out, err) = run(&[&base[..], &["archive", &target]].concat());
    assert!(ok, "the connected batch: {err}");
    assert!(out.contains("records   1"), "{out}");
    assert!(
        out.contains("newestness is the holder's claim"),
        "it does not overclaim: {out}"
    );
    // two records that both point at genesis are not the chain claimed
    *disconnected.lock().unwrap() = true;
    let (ok, _, err) = run(&[&base[..], &["archive", &target]].concat());
    assert!(!ok, "a batch that is not a chain is refused");
    assert!(err.contains("not a chain"), "naming what it found: {err}");
    assert!(
        err.contains(&hex(&rhtn_archive::record::Record::parse(&d2)
            .unwrap()
            .txid)),
        "and which record: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
