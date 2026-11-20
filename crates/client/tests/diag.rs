//! The field-test build's redaction, over the meetings this crate's tests
//! make (`Robot/field-test-diagnostics.md`, section 2): every event two
//! meetings, an adoption, a sealed exchange and a recovery raise is
//! rendered by the field build's own renderer and collected; then every
//! secret those runs produced is searched for in the rendering, eight
//! bytes at a time, as hex in either case and as a `Debug` byte list.
//! Not one run may appear.  The timeline is then checked for the step
//! sequence the plan's section 3 names.
//!
//! **The conversation's own events are swept where it runs**
//! (`adaptors/tests/redaction.rs`): the queries, the grants, the answers
//! and the body were rendered here while a driver gathered the signatures
//! in this process, and that driver is gone [author, 2026-10-14].
//!
//! Built and run only in the field-test flavour, which is the one that has
//! events: `cargo test -p rhtn-client --no-default-features --features
//! fieldtest --test diag`.  The releasable flavour compiles this file to
//! nothing, as it compiles the hooks to nothing.
#![cfg(feature = "fieldtest")]

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::node_key::NodeKeyRequest;
use rhtn_archive::sealed::Sealed;
use rhtn_archive::submission::relayed;
use rhtn_client::ceremony::Msg;
use rhtn_client::diag::json::JsonLines;
use rhtn_client::keys::capture_key;
use rhtn_client::payload::KIND_APPLICATION;
use rhtn_client::query::{KeyGrant, QueryRequest};
use rhtn_client::rotation::Rotation;
use rhtn_codec::cose::sha256;
use rhtn_node::node_keys::{NodeKeyConfig, NodeKeyService};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing_subscriber::layer::SubscriberExt;

const PARTIES: [&str; 6] = ["alice", "alice2", "bob", "carol", "w1", "w2"];

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

/// One message each way between `from` and `to` under the split seal,
/// each holding the other's introduction and the node `node`'s part played
/// by hand: its key for the recipient fetched, the submission queued, and
/// the node's half handed over with it. The halves the node handed are
/// returned, since they are secrets the rendering must not show.
fn exchange(s: &mut Setup, from: &str, to: &str, node: &str) -> Vec<Vec<u8>> {
    for (a, b) in [(from, to), (to, from)] {
        let card = s.client(a).own_introduction().expect("an introduction");
        s.client(b).take_introduction(&card).expect("taken");
    }
    let mut keys = NodeKeyService::new(NodeKeyConfig::default());
    let mut halves = Vec::new();
    let mut position = 0;
    for (a, b, text) in [(from, to, "over the seal"), (to, from, "and back")] {
        let asked = s
            .client(a)
            .send_payload(kh(b), KIND_APPLICATION, text.as_bytes())
            .expect("waits for a node key");
        assert!(
            matches!(asked.as_slice(), [Msg::NodeKeyRequest(r)] if *r == vec![kh(b)]),
            "a node key asked for"
        );
        let now = s.clock.get() / 1000;
        let req = NodeKeyRequest::for_recipients(vec![kh(b)], [position as u8; 16]).unwrap();
        let reply = keys
            .answer(&id(node), &|_| true, &req.encode(), now)
            .expect("answered");
        let out = s.client(a).take_node_key_reply(&kh(node), &[kh(b)], &reply);
        for m in out {
            let Msg::Sealed { sealed, .. } = m else {
                panic!("not sealed: {}", rhtn_client::diag::msg(&m))
            };
            let read = Sealed::decode(&sealed).expect("a seal");
            let ss_n = keys
                .open_half(&kh(b), read.n, &read.ct_n)
                .expect("the node's half");
            halves.push(ss_n.to_vec());
            position += 1;
            let (sender, _) = s
                .client(b)
                .receive_relayed(kh(node), &relayed(position, &sealed, &ss_n))
                .expect("opens at the recipient");
            assert_eq!(sender, kh(a));
        }
    }
    halves
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

fn in_order(names: &[String], expected: &[&str]) {
    let mut from = 0;
    for e in expected {
        match names[from..].iter().position(|n| n == e) {
            Some(i) => from += i + 1,
            None => panic!("{e} is not in the timeline after position {from}"),
        }
    }
}

// functional: OPS-014
// acceptance: none; a test facility's own test (Robot/field-test-diagnostics.md, section 6)
#[test]
fn no_secret_of_a_whole_ceremony_appears_in_any_event_and_the_timeline_is_complete() {
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = lines.clone();
    let subscriber = tracing_subscriber::registry().with(JsonLines::new(move |line| {
        sink.lock().unwrap().push(line);
    }));
    // two ceremonies, an adoption, a sealed exchange, two recovery
    // meetings and the recovery adoption, all rendered
    let (s, halves) = tracing::subscriber::with_default(subscriber, || {
        let mut s = setup(&PARTIES);
        s.met_in_person("alice", "bob", "w2");
        s.met_in_person("alice", "carol", "w2");
        s.adopted("alice", "w1", "w2", 3);
        let halves = exchange(&mut s, "alice", "bob", "w1");
        let old_archive = s.client("alice").archive.clone();
        s.client("alice2").rotation = Some(Box::new(Rotation::begin(
            Box::new(id("alice")),
            old_archive,
        )));
        s.face_off("alice2", "bob");
        s.h.run_recovery_meeting(kh("alice2"), kh("bob"))
            .expect("bob recognises");
        s.face_off("alice2", "carol");
        s.h.run_recovery_meeting(kh("alice2"), kh("carol"))
            .expect("carol recognises");
        s.h.run_recovery_adoption(kh("alice2"), kh("w1"), 7)
            .expect("adopted");
        (s, halves)
    });
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
        assert!(v.get("ms").is_some_and(|m| m.is_u64()), "ms: {v}");
        assert!(v.get("layer").is_some_and(|l| l.is_string()), "layer: {v}");
        assert!(v.get("event").is_some_and(|e| e.is_string()), "event: {v}");
    }

    // everything secret the runs produced: identity seeds, the identity
    // roots and the capture keys they derive, the encryption secrets, the
    // node's halves, and what crossed on the harness: the keys in flight,
    // the keys granted, the consents and the profiles
    let mut secrets: Vec<(&'static str, Vec<u8>)> = Vec::new();
    for n in PARTIES {
        for seed in identity_seeds(n) {
            secrets.push(("identity seed", seed));
        }
    }
    for (k, c) in &s.h.clients {
        // the capture keys derive from the identity root (design §7.5.2.6)
        let name = PARTIES.iter().find(|n| kh(n) == *k).expect("a party");
        let me = id(name);
        let root = me.identity_root();
        secrets.push(("identity root", root.to_vec()));
        for own in c.store.own.values() {
            secrets.push((
                "capture key",
                capture_key(root, k, &own.counterparty, &own.ceremony_id).to_vec(),
            ));
        }
        if let Some((kem_seed, auth)) = c.encryption_secrets() {
            secrets.push(("encryption seed", kem_seed.to_vec()));
            secrets.push(("sealing-authentication key", auth.to_vec()));
        }
    }
    for h in halves {
        secrets.push(("node half", h));
    }
    for sent in &s.h.log {
        match &sent.msg {
            Msg::CaptureKey(k) => secrets.push(("capture key in flight", k.to_vec())),
            Msg::Grant(b) => secrets.push((
                "granted key",
                KeyGrant::decode(b).expect("a grant").key.to_vec(),
            )),
            Msg::Consent { consent, .. } => secrets.push(("consent", consent.clone())),
            Msg::ConsentRequest(q) => secrets.push(("profile", q.profile.clone())),
            Msg::Query(b) => {
                let r = QueryRequest::decode(b).expect("a request");
                secrets.push(("profile", r.query.profile.clone()));
                secrets.push(("consent", r.consent.clone()));
            }
            _ => {}
        }
    }
    assert!(
        secrets
            .iter()
            .filter(|(w, _)| *w == "encryption seed")
            .count()
            == PARTIES.len(),
        "every party's encryption secrets"
    );
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

    // the timeline: the step sequence of a ceremony, then the exchange, the
    // recovery meeting's close and the seal on its way
    let names: Vec<String> = parsed
        .iter()
        .map(|v| v["event"].as_str().unwrap_or("").to_string())
        .collect();
    in_order(
        &names,
        &[
            "cer.begin",
            "cer.intent.received",
            "cer.id_fixed",
            "cer.capture",
            "cer.finalize",
            "pay.send",
            "pay.receive",
            "pay.send",
            "pay.receive",
            "cer.abandon",
        ],
    );
    assert!(
        lines.iter().any(|l| l.contains("\"msg\":\"Seal\"")),
        "the recovery's seal travelled on the harness"
    );
    let aborts: Vec<&serde_json::Value> = parsed
        .iter()
        .filter(|v| v["event"] == "cer.abort" || v["event"] == "pay.error")
        .collect();
    assert!(
        aborts.is_empty(),
        "a clean run aborts nothing and loses no payload: {aborts:?}"
    );

    // the histogram, for `--nocapture`: the hooks that fired, by name
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &names {
        *counts.entry(n.as_str()).or_default() += 1;
    }
    eprintln!("{} events over {} names", names.len(), counts.len());
    for (n, c) in &counts {
        eprintln!("  {c:>5}  {n}");
    }
}
