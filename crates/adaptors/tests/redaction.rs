//! **The field-test build's redaction over a whole ceremony**
//! (`Robot/field-test-diagnostics.md`, section 2): a meeting with a
//! verifier and a witness, run by the kernel over real sessions beside a
//! node, with every event the field build raises rendered by its own
//! renderer and collected; then every secret the run produced is searched
//! for in the rendering, eight bytes at a time, as hex in either case and
//! as a `Debug` byte list.  Not one run may appear.
//!
//! Built and run only in the diagnostics flavour, which is the one that
//! has events: `cargo test -p rhtn-adaptors --no-default-features
//! --features fieldtest --test redaction`. The releasable flavour
//! compiles this file to nothing, as it compiles the hooks to nothing.
//!
//! **Why here** [author, 2026-10-14]: the conversation's own events — the
//! selection, the query, the grant, the answer, the body and the
//! signatures — are raised where the conversation runs, and it runs over
//! the payload path. The client crate's copy of this sweep covers what
//! crosses a local interface and the recovery.
#![cfg(feature = "fieldtest")]

mod common;

use common::*;
use rhtn_client::device::ChannelKind;
use rhtn_client::diag::json::JsonLines;
use rhtn_codec::cose::sha256;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing_subscriber::layer::SubscriberExt;

/// The two seeds `test_identity` derives every party from.
fn identity_seeds(name: &str) -> Vec<Vec<u8>> {
    vec![
        sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes()).to_vec(),
        sha256(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes()).to_vec(),
    ]
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Every eight-byte run the rendering could be showing, as lowercase hex:
/// each 16-character window of every run of hex digits sixteen or longer,
/// in either case, and each eight-byte window of every `Debug` byte list.
fn observed_runs(text: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_hexdigit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_hexdigit() {
                i += 1;
            }
            let run = text[start..i].to_ascii_lowercase();
            if run.len() >= 16 {
                for w in 0..=run.len() - 16 {
                    seen.insert(run[w..w + 16].to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    for (at, _) in text.match_indices('[') {
        let rest = &text[at + 1..];
        let Some(end) = rest.find(']') else { continue };
        let nums: Option<Vec<u8>> = rest[..end]
            .split(',')
            .map(|n| n.trim().parse::<u8>().ok())
            .collect();
        if let Some(bytes) = nums
            && bytes.len() >= 8
        {
            for w in bytes.windows(8) {
                seen.insert(hex(w));
            }
        }
    }
    seen
}

const PARTIES: [&str; 5] = ["alice", "bob", "carol", "w1", "w2"];

// acceptance: none; a test facility's own test (Robot/field-test-diagnostics.md, section 6)
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn no_secret_of_a_whole_ceremony_appears_in_any_event() {
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = lines.clone();
    // **global, not thread-local**: each client runs on a thread of its
    // own and the node on the runtime's, and a subscriber set for this
    // thread alone would collect almost nothing
    let subscriber = tracing_subscriber::registry().with(JsonLines::new(move |line| {
        sink.lock().unwrap().push(line);
    }));
    tracing::subscriber::set_global_default(subscriber).expect("the only test in this binary");

    // the scene: alice has a record with w2 from before, meets bob in
    // person so he holds a capture of her, then meets carol, who selects
    // him as alice's verifier and asks him about her
    let mut prior = Scene::new();
    prior.meet("alice", "w2");
    let mut m = Meeting::open(
        vec![
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
            Who::with("carol", &[ChannelKind::Nfc]),
            Who::watching("w1"),
            Who::watching("w2"),
            Who::watching("witness"),
        ],
        "witness",
        prior.table("witness", &["witness"]),
    )
    .await;
    m.holds("alice", &prior, &["w2"]).await;
    m.holds("w2", &prior, &[]).await;
    m.meet("alice", "bob", &["w1"], &[])
        .await
        .expect("alice meets bob");
    m.in_horizon("carol", &["bob"]).await;
    m.meet("alice", "carol", &["w1"], &[])
        .await
        .expect("alice meets carol");

    // everything secret the run produced: the identity seeds, the ceremony
    // seeds and the capture keys they derive, and the payload material and
    // the sessions as they go to storage
    let mut secrets: Vec<(&'static str, Vec<u8>)> = Vec::new();
    for n in PARTIES {
        for seed in identity_seeds(n) {
            secrets.push(("identity seed", seed));
        }
    }
    for n in PARTIES {
        let k = kh(n);
        let mine = m
            .party(n)
            .with(move |c| {
                let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();
                for seed in c.store.seeds.values() {
                    out.push(("ceremony seed", seed.seed.to_vec()));
                    out.push((
                        "capture key",
                        rhtn_client::keys::capture_key(
                            &seed.seed,
                            &k,
                            &seed.counterparty,
                            &seed.ceremony_id,
                        )
                        .to_vec(),
                    ));
                }
                out.push(("payload material", c.payload.keys.encode()));
                out.push(("sessions", c.payload.sessions.encode()));
                out
            })
            .await;
        secrets.extend(mine);
    }
    assert!(
        secrets.iter().filter(|(w, _)| *w == "capture key").count() >= 4,
        "the meetings sealed captures and the seeds are held: {}",
        secrets.len()
    );

    let lines = lines.lock().unwrap().clone();
    assert!(
        lines.len() > 100,
        "{} events is not a timeline",
        lines.len()
    );
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("not one JSON object: {e}: {l}"))
        })
        .collect();
    for v in &parsed {
        assert!(v.get("ms").is_some_and(|x| x.is_u64()), "ms: {v}");
        assert!(v.get("layer").is_some_and(|x| x.is_string()), "layer: {v}");
        assert!(v.get("event").is_some_and(|x| x.is_string()), "event: {v}");
    }
    let text = lines.join("\n");
    let seen = observed_runs(&text);
    let mut hits = Vec::new();
    for (what, bytes) in &secrets {
        for w in bytes.windows(8) {
            let h = hex(w);
            if seen.contains(&h) {
                hits.push(format!("{what}: {h}"));
            }
        }
    }
    hits.sort();
    hits.dedup();
    assert!(hits.is_empty(), "secret runs rendered: {hits:?}");

    // **every hook of the conversation fired**: the steps the plan's
    // section 3 names, each at least once. Their order is the conversation
    // test's assertion (`conversation.rs`, kinds 9 to 18); here two
    // parties and two meetings run at once, and an interleaving of the
    // same steps is not a fault
    let names: Vec<&str> = parsed
        .iter()
        .map(|v| v["event"].as_str().unwrap_or(""))
        .collect();
    for want in [
        "cer.begin",
        "cer.intent.received",
        "cer.id_fixed",
        "cer.proximity",
        "cer.proximity.done",
        "cer.capture",
        "cer.select",
        "cer.query.issued",
        "sub.consented",
        "sub.grant",
        "ver.grant",
        "ver.query.answered",
        "cer.response",
        "sub.response_copy",
        "cer.witness.asked",
        "cer.witness.answered",
        "cer.propose",
        "cer.review",
        "cer.finalize",
        "pay.session",
    ] {
        assert!(names.contains(&want), "{want} was never raised");
    }
    assert!(
        !names.iter().any(|n| *n == "cer.abort" || *n == "pay.error"),
        "a clean run aborts nothing and loses no payload: {:?}",
        parsed
            .iter()
            .filter(|v| v["event"] == "cer.abort" || v["event"] == "pay.error")
            .collect::<Vec<_>>()
    );

    // the histogram, for `--nocapture`: the hooks that fired, by name
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &names {
        *counts.entry(n).or_default() += 1;
    }
    eprintln!("{} events over {} names", names.len(), counts.len());
    for (n, c) in &counts {
        eprintln!("  {c:>5}  {n}");
    }
}
